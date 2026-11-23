use super::*;
use crate::source_state::data::{DataDefinition, DataField, DataVariant, TypeParameter};
use crate::source_state::name::Identifier;
use crate::source_symbols::SymbolTableBuilder;
use symbols::SymbolNameRef;

/// `data Chain<T> { case Link(head: T, tail: Chain<T>); }` with the
/// references `Chain<T>` (the member's own type) and `Chain<u8>`.
fn chain() -> (TypedTrees, TypeReferenceHandle, TypeReferenceHandle) {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = builder.insert_children(
        root,
        crate::source_symbols::builtin_type_symbols()
            .into_iter()
            .chain([(SymbolKind::Data, SymbolNameRef::Static("Chain"))]),
    );
    let data = children.handle_at(symbols::BUILTIN_TYPE_COUNT as u32);
    let formal = builder
        .insert_children(
            data,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
        )
        .start();
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    let named = |program: &mut TypedTrees, symbol, name: &'static str| {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated(name),
            })
    };
    let applied = |program: &mut TypedTrees, argument| {
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles([argument]);
        program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: data,
                base_name: Identifier::generated("Chain"),
                arguments,
                lifetime_arguments: Vec::new(),
            })
    };
    let parameter = named(&mut program, formal, "T");
    let byte = named(
        &mut program,
        children.handle_at(symbols::BuiltinTypeAtom::U8.ordinal() as u32),
        "u8",
    );
    let open = applied(&mut program, parameter);
    let closed = applied(&mut program, byte);
    let type_parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: formal,
        name: Identifier::generated("T"),
        kind: TypeParameterKind::Type,
        ..Default::default()
    }]);
    let payload = program.tables.data_payload_fields.insert_many([
        DataField {
            name: Identifier::generated("head"),
            type_reference: parameter,
            ..Default::default()
        },
        DataField {
            name: Identifier::generated("tail"),
            type_reference: open,
            ..Default::default()
        },
    ]);
    let members = program
        .tables
        .data_members
        .insert_many([DataMember::Variant(DataVariant {
            name: Identifier::generated("Link"),
            payload,
            ..Default::default()
        })]);
    program.push_data_definition(DataDefinition {
        symbol: data,
        name: Identifier::generated("Chain"),
        type_parameters,
        members,
        ..Default::default()
    });
    (program, open, closed)
}

#[test]
fn data_that_stores_its_own_application_keeps_one_view_of_it() {
    let (mut program, open, closed) = chain();
    prepare_from(&mut program, 0).unwrap();
    for (reference, is_closed) in [(open, false), (closed, true)] {
        let view = program
            .data_application_definition(reference)
            .expect("the application retains its view");
        let DataMember::Variant(link) = &program.data_members(view)[0] else {
            panic!("the Link case")
        };
        // The `tail` member of `Chain<T>` is that same reference; the
        // one of `Chain<u8>` is an equal application with its own view.
        let tail = program.data_payload_fields(link)[1].type_reference;
        if reference == open {
            assert_eq!(tail, reference);
        }
        assert!(program.data_application_definition(tail).is_some());
        assert_eq!(prepare(&mut program, reference), Ok(is_closed));
    }
}

#[test]
fn a_type_that_contains_itself_with_no_declaration_between_is_refused() {
    let (mut program, _, closed) = chain();
    let looped = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice {
            element_type: closed,
        });
    program.type_reference_table.substitute_node(
        looped,
        TypeReferenceNode::Slice {
            element_type: looped,
        },
    );
    assert_eq!(
        prepare(&mut program, looped),
        Err("data application has a cyclic type-reference graph".into())
    );
}

#[test]
fn an_unresolved_type_reports_its_source_name() {
    let mut program = TypedTrees::default();
    let missing = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated("DirectoryStep"),
        });
    assert_eq!(
        prepare(&mut program, missing),
        Err("unknown type `DirectoryStep`".into())
    );
}
