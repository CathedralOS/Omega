use super::*;

#[test]
fn selected_record_values_keep_exact_names_and_require_closed_carrier_applications() {
    use crate::source_state::{
        data::{TypeParameter, TypeParameterKind},
        name::Identifier,
        trait_definition::{Conformance, TraitDefinition},
    };
    use crate::source_symbols::SymbolTableBuilder;
    use symbols::{SymbolKind, SymbolNameRef};

    let mut symbols = SymbolTableBuilder::default();
    let schema = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Static("Schema"));
    let other = symbols.insert_root(SymbolKind::Trait, SymbolNameRef::Static("Other"));
    let record = symbols.insert_root(SymbolKind::Conformance, SymbolNameRef::Static("Map"));
    let open = symbols.insert_root(SymbolKind::Conformance, SymbolNameRef::Static("Open"));
    let missing = symbols.insert_root(SymbolKind::Conformance, SymbolNameRef::Static("Missing"));
    let nonvalue = symbols.insert_root(SymbolKind::Local, SymbolNameRef::Static("local"));
    let owner = symbols.insert_root(SymbolKind::Machine, SymbolNameRef::Static("owner"));
    let parameter = symbols
        .insert_children(
            schema,
            [(SymbolKind::TypeParameter, SymbolNameRef::Static("T"))],
        )
        .start();
    let mut program = TypedTrees {
        symbols: symbols.finish(),
        ..Default::default()
    };
    let parameters = program.data_type_parameters.insert_many([TypeParameter {
        symbol: parameter,
        name: Identifier::generated("T"),
        kind: TypeParameterKind::Type,
        ..Default::default()
    }]);
    program.push_trait_definition(TraitDefinition {
        symbol: schema,
        name: Identifier::generated("Schema"),
        type_parameters: parameters,
        ..Default::default()
    });
    program.push_trait_definition(TraitDefinition {
        symbol: other,
        name: Identifier::generated("Other"),
        ..Default::default()
    });
    let unit = program
        .type_reference_table
        .insert(types::TypeReferenceNode::Unit);
    let slice = program
        .type_reference_table
        .insert(types::TypeReferenceNode::Slice { element_type: unit });
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles([unit]);
    for (symbol, lifetime_parameters) in [
        (record, Vec::new()),
        (open, vec![Identifier::generated("a")]),
    ] {
        program.push_conformance(Conformance {
            symbol,
            trait_symbol: schema,
            trait_name: Identifier::generated("Schema"),
            arguments,
            lifetime_parameters,
            ..Default::default()
        });
    }
    let named = |program: &mut TypedTrees, symbol| {
        let name = Identifier::from(program.symbols.name(symbol));
        program
            .type_reference_table
            .insert(types::TypeReferenceNode::Named { symbol, name })
    };
    let selected = named(&mut program, record);
    let open = named(&mut program, open);
    let missing = named(&mut program, missing);
    let nonvalue = named(&mut program, nonvalue);
    let other = named(&mut program, other);
    let application = |program: &mut TypedTrees, argument| {
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles([argument]);
        program
            .type_reference_table
            .insert(types::TypeReferenceNode::Generic {
                base_symbol: schema,
                base_name: Identifier::generated("Schema"),
                lifetime_arguments: Vec::new(),
                arguments,
            })
    };
    let carrier = application(&mut program, unit);
    let wrong_application = application(&mut program, slice);
    let value =
        CallableApplication::selected_value_expression(&mut program, selected, carrier, owner)
            .expect("the exact closed operation record is a named value");
    let name = program.expression_table.name_path_node(value).unwrap();
    assert_eq!(name.symbol, record);
    assert_eq!(name.head_symbol, record);
    for carrier in [
        types::TypeReferenceHandle::invalid(),
        unit,
        other,
        wrong_application,
    ] {
        assert!(
            CallableApplication::selected_value_expression(&mut program, selected, carrier, owner,)
                .is_err(),
            "the record needs its exact declared carrier application"
        );
    }
    for selected in [open, missing, nonvalue] {
        assert!(
            CallableApplication::selected_value_expression(&mut program, selected, carrier, owner,)
                .is_err(),
            "a schema cannot admit an open, undeclared, or nonvalue name"
        );
    }
}

#[test]
fn supplied_operand_does_not_acquire_preexisting_region_node() {
    let mut program = TypedTrees::default();
    let unit = program
        .type_reference_table
        .insert(types::TypeReferenceNode::Unit);
    let incoming = program
        .type_reference_table
        .insert(types::TypeReferenceNode::Reference {
            referee: unit,
            access: language_core::ReferenceAccess::Shared,
            lifetime: Some(crate::source_state::name::Identifier::generated("incoming")),
        });
    let unrelated = program
        .type_reference_table
        .insert(types::TypeReferenceNode::Reference {
            referee: unit,
            access: language_core::ReferenceAccess::Shared,
            lifetime: Some(crate::source_state::name::Identifier::generated(
                "unrelated",
            )),
        });
    let owner = symbols::SymbolHandle::from_parts(1, 1);
    let selected = expression::StaticMachineArgument {
        type_reference: incoming,
        ..Default::default()
    };
    let copied = program
        .static_operand_type_reference_in_scope(&selected, owner)
        .unwrap();
    assert!(
        !program
            .type_reference_table
            .operand_lifetime_owner(unrelated)
            .is_valid()
    );
    assert!(
        !program
            .type_reference_table
            .operand_lifetime_owner(incoming)
            .is_valid()
    );
    assert_eq!(
        program.type_reference_table.operand_lifetime_owner(copied),
        owner
    );
}
