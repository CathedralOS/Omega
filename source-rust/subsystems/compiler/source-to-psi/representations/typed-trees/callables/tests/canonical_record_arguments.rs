use crate::source_state::data::{DataDefinition, DataField, DataMember};
use crate::source_state::expression::{
    ExpressionNode, StaticMachineArgument, TableStructLiteral, TableStructLiteralField,
};
use crate::source_state::name::Identifier;
use crate::source_state::type_identity::package_policy_reading_tests::fixture::Fixture;
use crate::source_state::types::TypeReferenceNode;
use application_identity::const_value::CanonicalConstValue;
use application_identity::const_value::DecodedCanonicalConstValue;
use symbols::SymbolKind;

#[test]
fn static_record_arguments_keep_ordinary_fields_and_exact_checked_selections() {
    let mut fixture = Fixture::new();
    let sources = source_admission::source_identity::toolchain_source_identities(
        fixture.program.symbols.source_files(),
    )
    .expect("fixture sources remain inside their toolchain root");
    let record = fixture.symbol("Record");
    let first = fixture.symbol("LIMIT");
    let second = fixture.symbol("WIDTH");
    let callable = fixture.callable();
    let owner = fixture.program.symbols.get(record).parent;
    let boolean = fixture
        .program
        .symbols
        .child_handles(owner)
        .unwrap()
        .find(|symbol| {
            fixture.program.symbols.builtin_type_atom(*symbol)
                == Some(symbols::BuiltinTypeAtom::Bool)
        })
        .unwrap();
    let boolean = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: boolean,
            name: Identifier::generated("bool"),
        });
    let carrier = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: record,
            name: Identifier::generated("Record"),
        });
    let fields = fixture.program.symbols.insert_generated_children(
        record,
        [
            (SymbolKind::Field, "operation"),
            (SymbolKind::Field, "enabled"),
        ],
    );
    let members = fixture.program.data_members.insert_many([
        DataMember::Field(DataField {
            symbol: fields.handle_at(0),
            name: Identifier::generated("operation"),
            type_reference: callable,
            ..Default::default()
        }),
        DataMember::Field(DataField {
            symbol: fields.handle_at(1),
            name: Identifier::generated("enabled"),
            type_reference: boolean,
            ..Default::default()
        }),
    ]);
    fixture.program.push_data_definition(DataDefinition {
        symbol: record,
        name: Identifier::generated("Record"),
        members,
        ..Default::default()
    });
    let mut arguments = Vec::new();
    let mut flags = Vec::new();
    for (symbol, enabled) in [(first, true), (second, false)] {
        let operation = fixture.machine_value();
        let ExpressionNode::MachineValue(selected) =
            fixture.program.expression_table.expression(operation)
        else {
            panic!("the retained checked selection");
        };
        let selected = fixture
            .program
            .canonical_static_callable_value(selected, &sources)
            .unwrap();
        let decoded = DecodedCanonicalConstValue::Record {
            type_name: fixture.program.display_type_reference(carrier),
            fields: vec![
                ("operation".into(), selected),
                (
                    "enabled".into(),
                    DecodedCanonicalConstValue::Boolean(enabled),
                ),
            ],
        };
        let value = CanonicalConstValue::from_decoded(&decoded, "").unwrap();
        let flag = fixture
            .program
            .expression_table
            .insert(ExpressionNode::Boolean(enabled));
        flags.push(flag);
        let initializers = fixture.program.expression_table.insert_struct_fields([
            TableStructLiteralField {
                name: Identifier::generated("operation"),
                field_symbol: fields.handle_at(0),
                value: operation,
            },
            TableStructLiteralField {
                name: Identifier::generated("enabled"),
                field_symbol: fields.handle_at(1),
                value: flag,
            },
        ]);
        let initializer = fixture
            .program
            .expression_table
            .insert(ExpressionNode::StructLiteral(TableStructLiteral {
                type_name: Identifier::generated("Record"),
                type_symbol: record,
                fields: initializers,
                ..Default::default()
            }));
        fixture
            .program
            .push_const_declaration(crate::source_state::constant::ConstDeclaration {
                symbol,
                declared_type: carrier,
                materialized_initializer: initializer,
                evaluated_value: Some(value.encoding.into()),
                ..Default::default()
            });
        arguments.push(StaticMachineArgument {
            symbol,
            ..Default::default()
        });
    }
    let first_value = fixture
        .program
        .canonical_aggregate_argument(carrier, &arguments[0])
        .unwrap();
    let second_value = fixture
        .program
        .canonical_aggregate_argument(carrier, &arguments[1])
        .unwrap();
    assert_ne!(
        first_value.identity(),
        second_value.identity(),
        "the ordinary bool is part of identity"
    );
    assert!(
        fixture
            .program
            .canonical_aggregate_argument(fixture.u64_type, &arguments[0])
            .is_err()
    );
    *fixture.program.expression_table.expression_mut(flags[0]) = ExpressionNode::Boolean(false);
    assert!(
        fixture
            .program
            .canonical_aggregate_argument(carrier, &arguments[0])
            .is_err(),
        "an encoded record cannot replace a checked initializer's ordinary field"
    );
}
